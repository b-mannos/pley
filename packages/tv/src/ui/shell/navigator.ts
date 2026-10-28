import { signal } from "@preact/signals";
import type { LibrarySort } from "../../api/types";
import type { PlayRequest } from "../../player/plan";

/** The app's pages (the Android app's PleyKeys). */
export type PageKey =
  | { name: "servers" }
  | { name: "link"; server: string }
  | { name: "profiles" }
  | { name: "home" }
  /** Every library; `sort`: sorting them (Settings), and back when that ends. */
  | { name: "libraries"; sort?: boolean }
  /** `sort`: a row's See all, which lists the library so (newest or highest first) until the view is changed. */
  | { name: "library"; id: number; tab?: LibraryTab; sort?: LibrarySort }
  | { name: "item"; id: number }
  | { name: "actions"; id: number; continueWatching?: boolean }
  | { name: "search"; query?: string }
  | { name: "person"; id: number }
  | { name: "collections"; libraryId?: number }
  | { name: "collection"; id: number }
  | { name: "artist"; id: number }
  | { name: "album"; id: number; trackId?: number }
  | { name: "playlists" }
  | { name: "playlist"; id: number }
  | { name: "addToPlaylist"; itemIds: number[]; title: string }
  /** New playlist; with `smart`, Save these filters (a library's listing). */
  | { name: "newPlaylist"; smart?: { libraryId: number; filter: string } }
  | { name: "settings" }
  | { name: "history" }
  | { name: "diagnostics" }
  | { name: "nowPlaying" }
  | { name: "player"; request: PlayRequest; pressedAt: number };

export type LibraryTab = "recommended" | "library" | "collections";

export interface Entry {
  key: PageKey;
  /** Unique per entry: pages keep their state by it. */
  id: number;
  /** The `data-fid` of the element focused last, so Back lands on it. */
  memory: string | null;
  /** What a page keeps while it is away (its tab, its query). */
  state: Record<string, unknown>;
}

let nextId = 1;

/**
 * One back stack of pages (the Android app's TvNavigator): `open` pushes,
 * `restart` starts over from a page (the rail), `back` pops.
 */
export class Navigator {
  readonly stack = signal<Entry[]>([]);

  get top(): Entry | null {
    const s = this.stack.value;
    return s[s.length - 1] ?? null;
  }

  get root(): Entry | null {
    return this.stack.value[0] ?? null;
  }

  open(key: PageKey): void {
    this.stack.value = this.stack.value.concat([{ key, id: nextId++, memory: null, state: {} }]);
  }

  /** The page in place of the one on top (an item page that turns out to be an artist). */
  replace(key: PageKey): void {
    const s = this.stack.value.slice(0, -1);
    this.stack.value = s.concat([{ key, id: nextId++, memory: null, state: {} }]);
  }

  /** Starts over from `key`; nothing changes when it is the only page already. */
  restart(key: PageKey): void {
    const s = this.stack.value;
    if (s.length === 1 && sameKey(s[0]!.key, key)) return;
    this.stack.value = [{ key, id: nextId++, memory: null, state: {} }];
  }

  /** Pops the top page; false when it is the only one. */
  pop(): boolean {
    const s = this.stack.value;
    if (s.length <= 1) return false;
    this.stack.value = s.slice(0, -1);
    return true;
  }
}

export function sameKey(a: PageKey, b: PageKey): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
