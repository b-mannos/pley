import type HlsType from "hls.js";

export type HlsConstructor = typeof HlsType;

let loading: Promise<HlsConstructor> | null = null;

/** Lazily load hls.js so it isn't part of the main bundle. */
export function loadHls(): Promise<HlsConstructor> {
  if (!loading) {
    loading = import("hls.js").then((m) => m.default);
    loading.catch(() => {
      loading = null;
    });
  }
  return loading;
}
