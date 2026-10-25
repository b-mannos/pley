import { useEffect } from "react";
import type { QueryClient } from "@tanstack/react-query";
import type { Params } from "react-router";
import { queries } from "@/api/hooks";
import type { ItemKind } from "@/api/types";
import { loadHls } from "@/lib/hls";
import { decodeId } from "@/lib/ids";
import { androidPlayer } from "@/lib/shell";

/** The player page's code (the /play route loads it on demand). */
export const loadPlayerPage = () => import("./PlayerPage");

/**
 * The /play route's data, fetched together with the page's code: the item.
 * Continue Watching and links straight to the player skip the item page
 * that would have fetched it, and the player waits for it. An item already
 * there, even an older copy, opens the player right away (the player
 * updates it as any page does). A failure isn't retried here, holding the
 * click up: the player page shows it and retries itself.
 */
export function playerData(prefetch: QueryClient["prefetchQuery"], params: Params): Promise<void>[] {
  const id = decodeId(params.id);
  return id ? [prefetch({ ...queries.item(id), staleTime: Infinity, retry: false })] : [];
}

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

/**
 * Props for a Play button: hovering or focusing it fetches the player, and
 * runs `also` (the server reading the video ahead).
 */
export function playIntent(
  kind: ItemKind,
  also?: () => void,
): { onPointerEnter?: () => void; onFocus?: () => void } {
  const video = VIDEO_KINDS.has(kind);
  if (!video && !also) return {};
  const intent = () => {
    if (video) preloadPlayer();
    also?.();
  };
  return { onPointerEnter: intent, onFocus: intent };
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
