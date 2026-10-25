import { useEffect } from "react";
import type { ItemKind } from "@/api/types";
import { loadHls } from "@/lib/hls";
import { androidPlayer } from "@/lib/shell";

/** The player page's code (the /play route loads it on demand). */
export const loadPlayerPage = () => import("./PlayerPage");

/** What plays in the video player (shows and seasons through an episode). */
const VIDEO_KINDS = new Set<ItemKind>(["movie", "episode", "extra", "show", "season"]);

/**
 * Fetch the player's code and hls.js before Play is pressed: on the first
 * play they would otherwise load after the click. Each loads once (later
 * calls get the module already there; a failed load is tried again on the
 * next call). In the Android app its own player plays, so hls.js isn't
 * needed there.
 */
export function preloadPlayer(): void {
  loadPlayerPage().catch(() => undefined);
  if (!androidPlayer()) loadHls().catch(() => undefined);
}

/** Props for a Play button: hovering or focusing it fetches the player. */
export function playIntent(kind: ItemKind): { onPointerEnter?: () => void; onFocus?: () => void } {
  return VIDEO_KINDS.has(kind) ? { onPointerEnter: preloadPlayer, onFocus: preloadPlayer } : {};
}

/** How long the browser may hold an idle preload back while it is busy. */
const IDLE_TIMEOUT_MS = 2_000;

/** Preload the player once the page is idle, on pages of things that play in it. */
export function usePreloadPlayerWhenIdle(kind: ItemKind | undefined): void {
  const video = kind !== undefined && VIDEO_KINDS.has(kind);
  useEffect(() => {
    if (!video) return;
    if (typeof window.requestIdleCallback === "function") {
      const handle = window.requestIdleCallback(preloadPlayer, { timeout: IDLE_TIMEOUT_MS });
      return () => window.cancelIdleCallback(handle);
    }
    // Safari has no idle callbacks: soon after the page shows.
    const timer = window.setTimeout(preloadPlayer, 500);
    return () => window.clearTimeout(timer);
  }, [video]);
}
