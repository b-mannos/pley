import { signal } from "@preact/signals";
import type { Api } from "../api/client";
import type { LiveProgram } from "../api/types";
import { chunkStarts, GUIDE_CHUNK, mergeGuide } from "./data";

/** A chunk asked for this long ago is asked for again when it shows. */
const STALE_MS = 5 * 60_000;
/** A chunk that failed waits this long before it is asked for again. */
const RETRY_MS = 15_000;

/**
 * The guide's programmes, asked for in six-hour chunks as the guide is
 * moved through time (the web's GuideGrid), counted from `origin`, where
 * the guide opens. Every channel's programmes come in each chunk, so a
 * lineup of thousands costs no more requests than one of ten.
 */
export class GuideStore {
  readonly origin: number;
  /** Programmes by channel id, in order. */
  readonly programs = new Map<number, LiveProgram[]>();
  /** Bumped as chunks come in: rows read `programs` again. */
  readonly version = signal(0);
  /** No chunk asked for has come yet. */
  readonly loading = signal(true);
  private readonly api: Api;
  private readonly asked = new Map<number, { at: number; ok: boolean; pending: boolean }>();

  constructor(api: Api, origin: number) {
    this.api = api;
    this.origin = origin;
  }

  /** Asks for the chunks covering `from`–`to` (and the one after) that it doesn't have fresh. */
  want(from: number, to: number): void {
    const now = Date.now();
    for (const start of chunkStarts(from, to + GUIDE_CHUNK / 2, this.origin)) {
      const a = this.asked.get(start);
      if (a && (a.pending || now - a.at < (a.ok ? STALE_MS : RETRY_MS))) continue;
      const entry = { at: now, ok: false, pending: true };
      this.asked.set(start, entry);
      this.api.liveGuide(start, start + GUIDE_CHUNK).then(
        (guide) => {
          entry.ok = true;
          entry.pending = false;
          entry.at = Date.now();
          mergeGuide(this.programs, guide);
          this.loading.value = false;
          this.version.value++;
        },
        () => {
          entry.pending = false;
          entry.at = Date.now();
          this.loading.value = false;
          this.version.value++;
        },
      );
    }
  }

  /** The guide changed on the server: everything is asked for again as it shows. */
  forget(): void {
    this.asked.clear();
    this.programs.clear();
    this.version.value++;
  }
}
