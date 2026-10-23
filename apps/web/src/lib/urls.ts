import type { Id } from "@/api/types";
import { encodeId } from "./ids";

/** Page URLs, with short ids (see `ids.ts`). `query` goes after the `?`. */
export const itemUrl = (id: Id) => `/item/${encodeId(id)}`;
export const libraryUrl = (id: Id, query?: string) => `/library/${encodeId(id)}${query ? `?${query}` : ""}`;
export const personUrl = (id: Id) => `/person/${encodeId(id)}`;
export const collectionUrl = (id: Id) => `/collection/${encodeId(id)}`;
export const playlistUrl = (id: Id) => `/playlists/${encodeId(id)}`;
