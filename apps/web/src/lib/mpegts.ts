import type MpegtsType from "mpegts.js";

export type Mpegts = typeof MpegtsType;

let loading: Promise<Mpegts> | null = null;

/** Lazily load mpegts.js (live channels played as they come) so it isn't part of the main bundle. */
export function loadMpegts(): Promise<Mpegts> {
  if (!loading) {
    loading = import("mpegts.js").then((m) => m.default);
    loading.catch(() => {
      loading = null;
    });
  }
  return loading;
}
