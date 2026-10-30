import type { LiveChannel, LiveGuide, LiveProgram } from "../api/types";
import { formatTimeOfDay } from "../format/format";

/**
 * Live TV's rules that draw nothing (docs/API.md, "Live TV"): what's on,
 * the filters, changing channels, the guide's chunks and cells, and the
 * timeshift's words. The web app's (apps/web/src/pages/livetv) on a TV.
 */

/** The guide's columns: half an hour each. */
export const SLOT = 1800;
/** The guide is asked for in chunks of six hours, as it comes into view. */
export const GUIDE_CHUNK = 6 * 3600;
/** How far ahead the guide goes from where it opens. */
export const GUIDE_SPAN = 48 * 3600;
/** A gap in a channel's guide shorter than this belongs to the programme before it. */
const MIN_GAP = 60;

/** Unix seconds now. */
export function nowSeconds(): number {
  return Date.now() / 1000;
}

export function airing(p: Pick<LiveProgram, "start" | "stop">, now: number): boolean {
  return p.start <= now && p.stop > now;
}

/**
 * What's on a channel at `now`, and what comes next. The channel list is
 * asked for again every minute, so its `now` may have just ended: its
 * `next` is on then.
 */
export function nowAndNext(c: Pick<LiveChannel, "now" | "next">, now: number): { on: LiveProgram | null; next: LiveProgram | null } {
  if (c.now && airing(c.now, now)) return { on: c.now, next: c.next };
  if (c.next && airing(c.next, now)) return { on: c.next, next: null };
  const next = [c.now, c.next].filter((p): p is LiveProgram => !!p && p.start > now)[0] ?? null;
  return { on: null, next };
}

/** How far a programme is at `now`, 0–1. */
export function programProgress(p: Pick<LiveProgram, "start" | "stop">, now: number): number {
  return Math.max(0, Math.min(1, (now - p.start) / Math.max(1, p.stop - p.start)));
}

/** "12 min left", "1 h 5 min left". */
export function timeLeft(p: Pick<LiveProgram, "stop">, now: number): string {
  const minutes = Math.max(1, Math.round((p.stop - now) / 60));
  if (minutes < 60) return `${minutes} min left`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `${h} h ${m} min left` : `${h} h left`;
}

/** "20:00" or "8:00 PM", as the TV writes the time. */
export function formatTime(ts: number, hours24: boolean): string {
  return formatTimeOfDay(new Date(ts * 1000), hours24);
}

/** "20:00 – 21:00". */
export function formatSpan(p: Pick<LiveProgram, "start" | "stop">, hours24: boolean): string {
  return `${formatTime(p.start, hours24)} – ${formatTime(p.stop, hours24)}`;
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** "Today", "Tomorrow", "Yesterday", or "Fri 30 Oct". */
export function formatGuideDay(ts: number, now: number): string {
  const day = (t: number) => new Date(t * 1000).toDateString();
  if (day(ts) === day(now)) return "Today";
  if (day(ts) === day(now + 86_400)) return "Tomorrow";
  if (day(ts) === day(now - 86_400)) return "Yesterday";
  const d = new Date(ts * 1000);
  return `${DAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

/** "S2 E14", when the guide numbers the episode. */
export function episodeText(p: Pick<LiveProgram, "season" | "episode">): string | null {
  if (p.season != null && p.episode != null) return `S${p.season} E${p.episode}`;
  if (p.episode != null) return `E${p.episode}`;
  return null;
}

/** "101 Pley News HD", or the name alone. */
export function channelTitle(c: Pick<LiveChannel, "number" | "name">): string {
  return c.number ? `${c.number} ${c.name}` : c.name;
}

// ---- filters

export type ChannelFilter = { kind: "all" } | { kind: "favorites" } | { kind: "group"; group: string };

export function filterKey(f: ChannelFilter): string {
  return f.kind === "group" ? `group:${f.group}` : f.kind;
}

export function parseFilter(key: string | null): ChannelFilter {
  if (key === "favorites") return { kind: "favorites" };
  if (key && key.indexOf("group:") === 0) return { kind: "group", group: key.slice(6) };
  return { kind: "all" };
}

/** The groups channels are in, in their order, with how many each has. */
export function groupsOf(channels: LiveChannel[]): { name: string; count: number }[] {
  const out: { name: string; count: number }[] = [];
  const at: Record<string, number> = {};
  for (const c of channels) {
    if (!c.group) continue;
    const i = at[c.group];
    if (i === undefined) {
      at[c.group] = out.length;
      out.push({ name: c.group, count: 1 });
    } else out[i]!.count++;
  }
  return out;
}

/** The filter in force: one whose group went, or Favourites with none, shows every channel. */
export function effectiveFilter(filter: ChannelFilter, channels: LiveChannel[]): ChannelFilter {
  if (filter.kind === "group" && !channels.some((c) => c.group === filter.group)) return { kind: "all" };
  if (filter.kind === "favorites" && !channels.some((c) => c.favorite)) return { kind: "all" };
  return filter;
}

export function filterChannels(channels: LiveChannel[], filter: ChannelFilter): LiveChannel[] {
  if (filter.kind === "favorites") return channels.filter((c) => c.favorite);
  if (filter.kind === "group") return channels.filter((c) => c.group === filter.group);
  return channels;
}

/** The channels watched last that are still here, newest first. */
export function recentChannels(channels: LiveChannel[], recent: number[], max: number): LiveChannel[] {
  const out: LiveChannel[] = [];
  for (const id of recent) {
    const c = channels.filter((x) => x.id === id)[0];
    if (c) out.push(c);
    if (out.length >= max) break;
  }
  return out;
}

// ---- changing channels

/** The channel `by` places from `id` in `list`, round the end; `null` with no list. */
export function stepChannel(list: LiveChannel[], id: number, by: number): LiveChannel | null {
  if (!list.length) return null;
  const at = list.findIndex((c) => c.id === id);
  if (at < 0) return list[0]!;
  const n = list.length;
  return list[(((at + by) % n) + n) % n]!;
}

/** The channel numbered `digits` ("7" finds "007" too); `null` when none is. */
export function channelByNumber(list: LiveChannel[], digits: string): LiveChannel | null {
  const exact = list.filter((c) => c.number === digits)[0];
  if (exact) return exact;
  const n = Number(digits);
  if (!isFinite(n)) return null;
  return list.filter((c) => c.number !== null && /^\d+$/.test(c.number) && Number(c.number) === n)[0] ?? null;
}

// ---- the guide

/** The starts of the chunks covering `from`–`to`, counted from `origin` and kept within the guide's span. */
export function chunkStarts(from: number, to: number, origin: number): number[] {
  const first = Math.max(0, Math.floor((from - origin) / GUIDE_CHUNK));
  const last = Math.min(GUIDE_SPAN / GUIDE_CHUNK - 1, Math.floor((Math.max(from, to - 1) - origin) / GUIDE_CHUNK));
  const out: number[] = [];
  for (let i = first; i <= last; i++) out.push(origin + i * GUIDE_CHUNK);
  return out;
}

/** A chunk's programmes added to what was had, each once (by its start), in order. */
export function mergeGuide(into: Map<number, LiveProgram[]>, guide: LiveGuide): void {
  for (const key in guide.programs) {
    const id = Number(key);
    const known = into.get(id) ?? [];
    const starts: Record<number, true> = {};
    for (const p of known) starts[p.start] = true;
    const added = (guide.programs[key] ?? []).filter((p) => !starts[p.start]);
    if (!added.length) {
      into.set(id, known);
      continue;
    }
    into.set(id, known.concat(added).sort((a, b) => a.start - b.start));
  }
}

/** One cell of a guide row: a programme, or a stretch the guide says nothing of (`program` null). */
export interface GuideCell {
  key: string;
  start: number;
  stop: number;
  program: LiveProgram | null;
}

/**
 * A channel's row between `from` and `to`: its programmes there, with what
 * the guide says nothing of between them as cells of their own, so the
 * remote moves through time without jumping over a hole. A channel without
 * a guide is one cell.
 */
export function guideCells(programs: LiveProgram[] | undefined, from: number, to: number): GuideCell[] {
  const out: GuideCell[] = [];
  let at = from;
  for (const p of programs ?? []) {
    if (p.stop <= from || p.stop <= at) continue;
    if (p.start >= to) break;
    if (p.start - at >= MIN_GAP) out.push({ key: `gap/${at}`, start: at, stop: p.start, program: null });
    const start = Math.max(p.start, at);
    out.push({ key: String(p.start), start, stop: p.stop, program: p });
    at = p.stop;
  }
  if (to - at >= MIN_GAP || !out.length) out.push({ key: `gap/${at}`, start: at, stop: Math.max(to, at + MIN_GAP), program: null });
  return out;
}

/** The cell at time `t` (the last one when it is past them all). */
export function cellAt(cells: GuideCell[], t: number): number {
  for (let i = 0; i < cells.length; i++) if (cells[i]!.stop > t) return i;
  return cells.length - 1;
}

// ---- the timeshift

/** Within this much of the live edge counts as live: the TVs' players stay a few segments behind it. */
export const LIVE_WITHIN_MS = 15_000;

/** How far behind live: "1:23", or "12 min" from ten minutes. */
export function behindText(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s >= 600) return `${Math.round(s / 60)} min`;
  const sec = s % 60;
  return `${Math.floor(s / 60)}:${sec < 10 ? "0" : ""}${sec}`;
}
