import type { Api } from "../../api/client";
import type { ItemSummary, LibraryKind, LibrarySort } from "../../api/types";
import type { KeyValue } from "../../auth/tokens";
import { Cancel } from "../../net/transport";

/** How a library is listed: sort, order and filters (the web's query string). */
export interface LibraryParams {
  sort: LibrarySort;
  order: "asc" | "desc";
  /** false: unwatched only; true: watched only; null: all. */
  watched: boolean | null;
  genre: string | null;
  year: number | null;
}

export const SORT_LABELS: Record<LibrarySort, string> = {
  title: "Title",
  year: "Year",
  added: "Date added",
  rating: "Rating",
  user_rating: "Your rating",
  release: "Release date",
  duration: "Duration",
  last_played: "Last played",
  bitrate: "Bitrate",
};

/** The sorts each kind of library offers, in order. */
export function sortsFor(kind: LibraryKind): LibrarySort[] {
  if (kind === "music") return ["title", "added", "user_rating", "last_played"];
  const all: LibrarySort[] = ["title", "year", "added", "rating", "user_rating", "release", "duration", "last_played", "bitrate"];
  return kind === "shows" ? all.filter((s) => s !== "duration") : all;
}

/** A sort's own order: A to Z for titles, newest or highest first for the rest. */
export function defaultOrder(sort: LibrarySort): "asc" | "desc" {
  return sort === "title" ? "asc" : "desc";
}

export function defaultParams(): LibraryParams {
  return { sort: "title", order: "asc", watched: null, genre: null, year: null };
}

export function hasFilter(p: LibraryParams): boolean {
  return p.watched !== null || p.genre !== null || p.year !== null;
}

/** Params as kept, made fit for the library: music has no filters; a sort the kind doesn't offer is Title. */
export function fitParams(p: Partial<LibraryParams> | null, kind: LibraryKind): LibraryParams {
  const d = defaultParams();
  const sort = p?.sort && sortsFor(kind).indexOf(p.sort) >= 0 ? p.sort : d.sort;
  const order = p?.order === "asc" || p?.order === "desc" ? p.order : defaultOrder(sort);
  if (kind === "music") return { sort, order, watched: null, genre: null, year: null };
  return {
    sort,
    order,
    watched: typeof p?.watched === "boolean" ? p.watched : null,
    genre: typeof p?.genre === "string" && p.genre ? p.genre : null,
    year: typeof p?.year === "number" ? p.year : null,
  };
}

/** Each library's sort and filters, kept on the TV so it opens as it was left (per server). */
export class ParamsStore {
  private readonly store: KeyValue;
  private readonly server: string;
  constructor(store: KeyValue, server: string) {
    this.store = store;
    this.server = server;
  }
  private key(id: number): string {
    return `pley.library.${this.server}.${id}`;
  }
  get(id: number, kind: LibraryKind): LibraryParams {
    let saved: Partial<LibraryParams> | null = null;
    try {
      const text = this.store.get(this.key(id));
      saved = text ? (JSON.parse(text) as Partial<LibraryParams>) : null;
    } catch {
      saved = null;
    }
    return fitParams(saved, kind);
  }
  set(id: number, p: LibraryParams): void {
    this.store.set(this.key(id), JSON.stringify(p));
  }
}

/** "1,500 movies", "1 show", "12 artists". */
export function countLabel(n: number, kind: LibraryKind): string {
  const noun = kind === "movies" ? (n === 1 ? "movie" : "movies") : kind === "shows" ? (n === 1 ? "show" : "shows") : n === 1 ? "artist" : "artists";
  return `${grouped(n)} ${noun}`;
}

/** 1500 → "1,500". */
export function grouped(n: number): string {
  const s = String(Math.round(n));
  return s.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// ---- A–Z

export const LETTERS = ["#", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];

/** An item's letter: the first of its sort title, accents removed; "#" for anything that isn't A–Z. */
export function letterOf(item: Pick<ItemSummary, "sort_title" | "title">): string {
  const source = (item.sort_title || item.title || "").trim();
  let c = source.charAt(0);
  try {
    c = c.normalize("NFD").charAt(0);
  } catch {
    // Old engines without normalize: the character as it is.
  }
  c = c.toUpperCase();
  return c >= "A" && c <= "Z" ? c : "#";
}

/** The first position of each letter in a listing, and the letter at any position. */
export class AlphaIndex {
  readonly firsts = new Map<string, number>();
  private readonly runs: { letter: string; start: number }[] = [];

  constructor(items: Pick<ItemSummary, "sort_title" | "title">[]) {
    let last: string | null = null;
    items.forEach((it, i) => {
      const l = letterOf(it);
      if (!this.firsts.has(l)) this.firsts.set(l, i);
      if (l !== last) {
        this.runs.push({ letter: l, start: i });
        last = l;
      }
    });
  }

  /** The letter of the run `position` is in. */
  letterAt(position: number): string {
    let lo = 0;
    let hi = this.runs.length - 1;
    let found = this.runs[0]?.letter ?? "#";
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (this.runs[mid]!.start <= position) {
        found = this.runs[mid]!.letter;
        lo = mid + 1;
      } else hi = mid - 1;
    }
    return found;
  }

  has(letter: string): boolean {
    return this.firsts.has(letter);
  }

  /** Where a letter jumps to: its first title, else the next letter's, else the nearest before it, else 0. */
  jumpTarget(letter: string): number {
    const at = this.firsts.get(letter);
    if (at !== undefined) return at;
    const i = LETTERS.indexOf(letter);
    for (let j = i + 1; j < LETTERS.length; j++) {
      const p = this.firsts.get(LETTERS[j]!);
      if (p !== undefined) return p;
    }
    for (let j = i - 1; j >= 0; j--) {
      const p = this.firsts.get(LETTERS[j]!);
      if (p !== undefined) return p;
    }
    return 0;
  }
}

/** The A–Z shows for title sorts of a listing held whole, of more than 30 titles. */
export function showsJump(p: LibraryParams, total: number | null, windowed: boolean): boolean {
  return p.sort === "title" && !windowed && total !== null && total > 30;
}

// ---- the listing

export const HEAD = 200;
export const PAGE = 1000;
/** Pages kept around the visible part of a big library. */
export const KEPT_PAGES = 4;

/**
 * A library's listing (the Android app's LibraryListing): the first 200
 * at once, then the rest in pages of 1000, two at a time; a library above
 * `windowAbove` keeps only the pages around what shows.
 */
export class Listing {
  total: number | null = null;
  error: unknown = null;
  readonly windowAbove: number;
  private readonly items: (ItemSummary | undefined)[] = [];
  private readonly pages = new Map<number, "loading" | "done">();
  private readonly order: number[] = [];
  private readonly cancel = new Cancel();
  private readonly api: Api;
  private readonly id: number;
  private readonly params: LibraryParams;
  private readonly onChange: () => void;

  constructor(api: Api, id: number, params: LibraryParams, windowAbove: number, onChange: () => void) {
    this.api = api;
    this.id = id;
    this.params = params;
    this.windowAbove = windowAbove;
    this.onChange = onChange;
  }

  get windowed(): boolean {
    return this.total !== null && this.total > this.windowAbove;
  }

  at(i: number): ItemSummary | undefined {
    return this.items[i];
  }

  /** Every item, when the whole listing is held; `null` otherwise. */
  all(): ItemSummary[] | null {
    if (this.total === null) return null;
    for (let i = 0; i < this.total; i++) if (!this.items[i]) return null;
    return this.items.slice(0, this.total) as ItemSummary[];
  }

  stop(): void {
    this.cancel.cancel();
  }

  private query(offset: number, limit: number) {
    const p = this.params;
    // In this order: the server's listing ETag hashes the query as written.
    return {
      sort: p.sort,
      order: p.order,
      watched: p.watched === null ? undefined : p.watched,
      genre: p.genre ?? undefined,
      year: p.year ?? undefined,
      offset,
      limit,
    };
  }

  private put(offset: number, list: ItemSummary[]): void {
    for (let i = 0; i < list.length; i++) this.items[offset + i] = list[i];
  }

  async start(): Promise<void> {
    try {
      const head = await this.api.libraryItems(this.id, this.query(0, HEAD), this.cancel);
      this.total = head.total;
      this.put(0, head.items);
      this.onChange();
      if (!this.windowed) await this.rest(HEAD);
      else this.showing(0);
    } catch (e) {
      if (this.cancel.cancelled) return;
      this.error = e;
      this.onChange();
    }
  }

  /** Everything after the head, two pages at a time; a short page ends it. */
  private async rest(from: number): Promise<void> {
    let offset = from;
    while (this.total !== null && offset < this.total && !this.cancel.cancelled) {
      const batch = [offset, offset + PAGE].filter((o) => o < (this.total ?? 0));
      const answers = await Promise.all(batch.map((o) => this.api.libraryItems(this.id, this.query(o, PAGE), this.cancel)));
      let short = false;
      answers.forEach((a, i) => {
        this.put(batch[i]!, a.items);
        if (a.items.length < PAGE) short = true;
      });
      this.onChange();
      if (short) return;
      offset += PAGE * 2;
    }
  }

  /** A big library: the pages around the first visible position, fetched when missing. */
  showing(first: number): void {
    if (!this.windowed || this.total === null) return;
    const page = Math.floor(first / PAGE);
    const start = first % PAGE < PAGE / 2 ? Math.max(0, page - 1) : page;
    const last = Math.floor((this.total - 1) / PAGE);
    for (const p of [start, start + 1]) {
      if (p > last || this.pages.has(p)) continue;
      this.pages.set(p, "loading");
      this.api.libraryItems(this.id, this.query(p * PAGE, PAGE), this.cancel).then(
        (a) => {
          this.pages.set(p, "done");
          this.put(p * PAGE, a.items);
          this.keep(p);
          this.onChange();
        },
        () => this.pages.delete(p),
      );
    }
  }

  /** At most four pages stay; the oldest others go (the head stays too). */
  private keep(p: number): void {
    const at = this.order.indexOf(p);
    if (at >= 0) this.order.splice(at, 1);
    this.order.push(p);
    while (this.order.length > KEPT_PAGES) {
      const old = this.order.shift()!;
      this.pages.delete(old);
      for (let i = Math.max(old * PAGE, HEAD); i < (old + 1) * PAGE; i++) this.items[i] = undefined;
    }
  }
}
