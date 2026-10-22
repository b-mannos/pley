import { api } from "@/api/client";
import type { Id, PlaybackInfo, PlaybackRequest, ProgressRequest } from "@/api/types";

/** Fire-and-forget progress report. `keepalive` lets it survive page unloads. */
export function reportProgress(itemId: Id, body: ProgressRequest, keepalive = false): Promise<void> {
  return api
    .post<void>(`/api/items/${itemId}/progress`, body, { keepalive, noAuthRedirect: keepalive })
    .catch(() => undefined);
}

export function stopSession(sessionId: string, keepalive = false): Promise<void> {
  return api
    .del<void>(`/api/playback/${encodeURIComponent(sessionId)}`, { keepalive, noAuthRedirect: keepalive })
    .catch(() => undefined);
}

export function startPlayback(req: PlaybackRequest, signal?: AbortSignal): Promise<PlaybackInfo> {
  return api.post<PlaybackInfo>("/api/playback", req, { signal });
}
