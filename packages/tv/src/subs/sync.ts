import type { KeyValue } from "../auth/tokens";

/**
 * Moving subtitles in time, for files whose subtitles are early or late:
 * positive is later (a subtitle at 10 s shows at 10.5 s with +0.5 s).
 * Steps of 100 ms, at most 10 s either way, remembered per file on this
 * TV and never sent to the server (the Android app's rules).
 */
export const SYNC_STEP_MS = 100;
export const SYNC_MAX_MS = 10_000;

export function clampOffset(ms: number): number {
  if (!isFinite(ms)) return 0;
  const stepped = Math.round(ms / SYNC_STEP_MS) * SYNC_STEP_MS;
  return Math.max(-SYNC_MAX_MS, Math.min(SYNC_MAX_MS, stepped)) + 0;
}

/** `+0.5 s`, `−1.2 s`, `0.0 s`. */
export function formatOffset(ms: number): string {
  const v = clampOffset(ms);
  const sign = v > 0 ? "+" : v < 0 ? "−" : "";
  return `${sign}${(Math.abs(v) / 1000).toFixed(1)} s`;
}

const KEY = "pley.subtitleOffsets";
/** Files whose offsets are kept: the oldest go first. */
const KEPT = 300;

/** Offsets by file, kept under the file's absolute address (`<server>/api/files/<id>`). */
export class OffsetStore {
  private readonly store: KeyValue;

  constructor(store: KeyValue) {
    this.store = store;
  }

  private read(): [string, number][] {
    try {
      const v: unknown = JSON.parse(this.store.get(KEY) ?? "[]");
      return Array.isArray(v) ? (v.filter((e) => Array.isArray(e) && typeof e[0] === "string" && typeof e[1] === "number") as [string, number][]) : [];
    } catch {
      return [];
    }
  }

  get(server: string, fileId: number): number {
    const key = `${server}/api/files/${fileId}`;
    for (const [k, v] of this.read()) if (k === key) return clampOffset(v);
    return 0;
  }

  /** Remembers `ms` for the file; 0 forgets it. */
  set(server: string, fileId: number, ms: number): void {
    const key = `${server}/api/files/${fileId}`;
    const entries = this.read().filter(([k]) => k !== key);
    const value = clampOffset(ms);
    if (value !== 0) entries.push([key, value]);
    this.store.set(KEY, JSON.stringify(entries.slice(-KEPT)));
  }
}
