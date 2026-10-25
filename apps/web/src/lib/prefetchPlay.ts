import { api } from "@/api/client";
import type { Id, ItemSummary } from "@/api/types";

/** Kinds whose Play starts a video (a show or season: its next episode). */
const VIDEO_KINDS = new Set<ItemSummary["kind"]>(["movie", "episode", "extra", "show", "season"]);
/** The server reads a file ahead at most this often anyway. */
const AGAIN_AFTER_MS = 2 * 60_000;
const KEPT = 500;

const asked = new Map<Id, number>();

/**
 * Someone is about to press Play on a card (its Play button has the pointer
 * or the focus): the server reads ahead where the video starts, so the first
 * seconds come from a cache, which matters for files on a network share.
 * At most once every two minutes per item; fire and forget.
 */
export function prefetchPlay(item: Pick<ItemSummary, "id" | "kind">, now = Date.now()): void {
  if (!VIDEO_KINDS.has(item.kind)) return;
  const last = asked.get(item.id);
  if (last !== undefined && now - last < AGAIN_AFTER_MS) return;
  if (asked.size >= KEPT) asked.clear();
  asked.set(item.id, now);
  // Only a hint: a failure (or a signed-out user) changes nothing.
  void api.post<void>(`/api/items/${item.id}/prefetch`, undefined, { noAuthRedirect: true }).catch(() => undefined);
}

/** Forget what was asked (tests). */
export function resetPrefetchPlay(): void {
  asked.clear();
}
