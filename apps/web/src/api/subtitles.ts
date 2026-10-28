import { api, isApiError, qs } from "./client";
import type { Id, MediaStream, SubtitleResult, SubtitleTrack } from "./types";

/** The largest subtitle file the server takes. */
export const SUBTITLE_FILE_MAX = 2 * 1024 * 1024;
/** What the server takes as subtitle files. */
export const SUBTITLE_FILE_TYPES = [".srt", ".vtt", ".ass", ".ssa"];

/** Subtitle files found for an item's file, in `languages` (two-letter codes; the profile's own when left out). */
export function searchSubtitles(itemId: Id, fileId: Id | null, languages: string[] | null, signal?: AbortSignal): Promise<SubtitleResult[]> {
  return api
    .get<{ results: SubtitleResult[] }>(
      `/api/items/${itemId}/subtitles/search${qs({ file_id: fileId, language: languages?.length ? languages.join(",") : null })}`,
      { signal },
    )
    .then((r) => r.results ?? []);
}

/** Adds a subtitle file found by `searchSubtitles`; the new track. */
export function addFoundSubtitle(itemId: Id, fileId: Id | null, result: string): Promise<SubtitleTrack> {
  return api.post<SubtitleTrack>(`/api/items/${itemId}/subtitles`, fileId === null ? { result } : { result, file_id: fileId });
}

/** Adds a subtitle file from this computer, in `language`; the new track. */
export function uploadSubtitle(
  itemId: Id,
  fileId: Id | null,
  file: File,
  language: string,
  flags: { forced?: boolean; hearingImpaired?: boolean } = {},
): Promise<SubtitleTrack> {
  const form = new FormData();
  form.append("file", file, file.name);
  form.append("language", language);
  if (fileId !== null) form.append("file_id", String(fileId));
  if (flags.forced) form.append("forced", "true");
  if (flags.hearingImpaired) form.append("hearing_impaired", "true");
  return api.post<SubtitleTrack>(`/api/items/${itemId}/subtitles`, undefined, { form });
}

/** Deletes an added subtitle track. */
export function deleteSubtitle(itemId: Id, index: number): Promise<void> {
  return api.del<void>(`/api/items/${itemId}/subtitles/${index}`);
}

/**
 * Why subtitles can't be searched for, from the search's error: not set
 * up on this server (503 with `setup: false`), or a server older than
 * subtitle search (404); `null` for any other failure.
 */
export function searchUnavailable(err: unknown): "setup" | "older" | null {
  if (isApiError(err, 404) && err.code === "not_found") return "older";
  if (isApiError(err, 503) && (err.body as { setup?: unknown } | undefined)?.setup === false) return "setup";
  return null;
}

/** An added track as the player's menus list streams. */
export function trackStream(track: SubtitleTrack): MediaStream {
  return {
    index: track.index,
    kind: "subtitle",
    codec: track.codec,
    language: track.language,
    title: track.title,
    default: track.default,
    forced: track.forced,
    external: true,
    text_based: track.text_based,
    added: track.added ?? true,
    added_by: track.added_by,
  };
}
