import type { ComponentChildren } from "preact";
import { useLayoutEffect, useRef, useState } from "preact/hooks";
import { useUi } from "../context";
import type { Dir } from "../focus";
import { G } from "./focusable";
import { TILES, type TileKind } from "./tile";

/** Every page's left edge, where a row keeps its focused tile. */
export const GUTTER = 96;
/** A row's tiles all fit when they end by here (960 − 48). */
const FIT_END = 912;

export const HEADER = 24;
export const HEADER_TO_TILES = 14;
export const TILES_TO_CAPTION = 14;
export const CAPTION = 34;
export const ROW_GAP = 28;
/** The canvas's height: a page draws its sections within one of it above and below what shows. */
const SCREEN = 540;

/** A row's height: header, tiles and caption. */
export function rowHeight(kind: TileKind, titled = true): number {
  return (titled ? HEADER + HEADER_TO_TILES : 0) + TILES[kind].h + TILES_TO_CAPTION + CAPTION;
}

/**
 * Where a row's track sits with tile `index` focused: its left edge at
 * the gutter, the row still when every tile fits, and never past the end
 * (the last tile's right edge at 960 − 48 − one tile).
 */
export function rowOffset(kind: TileKind, count: number, index: number): number {
  const { w, gap } = TILES[kind];
  const span = count * w + Math.max(0, count - 1) * gap;
  if (GUTTER + span <= FIT_END) return 0;
  const content = GUTTER + span + 48 + w;
  const max = Math.max(0, content - 960);
  return Math.max(0, Math.min(index * (w + gap), max));
}

/** Tiles drawn on either side of the screen: the next one and a half screens ahead load first. */
function windowOf(kind: TileKind, count: number, offset: number): [number, number] {
  const { w, gap } = TILES[kind];
  const pitch = w + gap;
  const perScreen = Math.ceil(960 / pitch);
  const first = Math.max(0, Math.floor((offset - GUTTER) / pitch) - 2);
  const last = Math.min(count, first + perScreen + Math.ceil(perScreen * 1.5) + 2);
  return [first, last];
}

export interface RowProps<T> {
  title?: string;
  kind: TileKind;
  items: T[];
  /** One tile; the row places it. */
  tile: (item: T, index: number, place: { onFocus: (el: HTMLElement) => void; priority: number }) => ComponentChildren;
  /** A stable key per tile (an item's id), so tiles keep their pictures when others come and go. */
  keyOf?: (item: T, index: number) => string | number;
  /** A "See all" tile after the items. */
  seeAll?: (place: { onFocus: (el: HTMLElement) => void }) => ComponentChildren;
  /** Left from the first tile: the rail. */
  onLeftEdge?: () => boolean;
  /** The tile focus enters at when the row has none to come back to (the one focused before Back). */
  enterAt?: number;
  onFocusIndex?: (index: number) => void;
  class?: string;
  style?: string;
}

/**
 * A row of tiles (the Android app's TvRow): a header, then a track that
 * slides under the focused tile, kept at x 96. One focus group that
 * returns to its own last tile; Left from the first tile goes to the
 * rail, Right at the end stops. Only the tiles near the screen are drawn.
 * When the focused tile leaves (its actions removed it), focus goes to
 * the tile now at its place, or the last.
 */
export function Row<T>(p: RowProps<T>) {
  const ui = useUi();
  const spec = TILES[p.kind];
  const count = p.items.length + (p.seeAll ? 1 : 0);
  const [focused, setFocused] = useState(Math.min(p.enterAt ?? 0, Math.max(0, count - 1)));
  const at = Math.min(focused, Math.max(0, count - 1));
  const offset = rowOffset(p.kind, count, at);
  const [first, last] = windowOf(p.kind, count, offset);
  const groupEl = useRef<HTMLElement | null>(null);
  const focusedEl = useRef<HTMLElement | null>(null);
  const place = (i: number) => {
    const onFocus = (el: HTMLElement) => {
      focusedEl.current = el;
      setFocused(i);
      p.onFocusIndex?.(i);
    };
    return { onFocus, priority: Math.abs(i - at) };
  };
  const onEdge = (dir: Dir) => {
    if (dir === "left") return p.onLeftEdge ? p.onLeftEdge() : false;
    return dir === "right";
  };
  const slot = (key: string | number, i: number, child: ComponentChildren) => (
    <div key={key} class="row-slot" data-index={i} style={`left:${GUTTER + i * (spec.w + spec.gap)}px`}>
      {child}
    </div>
  );
  const tiles: ComponentChildren[] = [];
  for (let i = first; i < Math.min(last, p.items.length); i++) {
    const item = p.items[i]!;
    tiles.push(slot(p.keyOf ? p.keyOf(item, i) : i, i, p.tile(item, i, place(i))));
  }
  if (p.seeAll && last >= count && first <= p.items.length) tiles.push(slot("see-all", p.items.length, p.seeAll(place(p.items.length))));
  // The focused tile went away: the one now in its place takes focus.
  useLayoutEffect(() => {
    const was = focusedEl.current;
    if (!was || was.isConnected || ui.focus.current !== was) return;
    const group = groupEl.current;
    if (!group || !count) return;
    const target = group.querySelector<HTMLElement>(`[data-index="${at}"] [data-f]`) ?? lastTile(group);
    if (target) ui.focus.focus(target);
  });
  const height = spec.h + TILES_TO_CAPTION + CAPTION;
  return (
    <div class={`row ${p.class ?? ""}`} style={p.style}>
      {p.title !== undefined && <div class="row-header t-section one-line">{p.title}</div>}
      <G
        kind="row"
        restore
        class="row-track-group"
        style={`position:relative;height:${height}px;margin-top:${p.title !== undefined ? HEADER_TO_TILES : 0}px`}
        onEdge={onEdge}
        onEnter={(_dir, from) => {
          const group = groupEl.current;
          if (!group || ui.focus.rememberedIn(group)) return null;
          // A row not visited on this page yet: the tile asked for, else the nearest (from above or below), else the first.
          if (p.enterAt !== undefined) return group.querySelector<HTMLElement>(`[data-index="${Math.min(p.enterAt, count - 1)}"] [data-f]`);
          return from ? null : group.querySelector<HTMLElement>("[data-f]");
        }}
        elRef={(el) => (groupEl.current = el)}
      >
        <div class="track" style={`position:relative;height:${height}px;transform:translateX(${-offset}px)`}>
          {tiles}
        </div>
      </G>
    </div>
  );
}

function lastTile(group: HTMLElement): HTMLElement | null {
  const all = group.querySelectorAll<HTMLElement>("[data-f]");
  return all.length ? all[all.length - 1]! : null;
}

export interface Section {
  key: string;
  /** Its height, header and caption included. */
  height: number;
  /** The y its focused content should sit at (a row's header line); `null`: scroll to the page's top. */
  anchor: number | null;
  render: (onFocus: () => void) => ComponentChildren;
}

/**
 * A page of sections, 28 apart (the Android app's TvRowColumn): the
 * focused section comes up so its header sits at its anchor (172 for
 * rows), a hero section brings the page back to its top, and once
 * scrolled nothing draws above the anchor line − 12. As the Android app's
 * lazy column, only the sections within a screen of what shows are drawn;
 * a row drawn again comes back to its tile by its `enterAt`.
 */
export function SectionPage(p: {
  sections: Section[];
  top?: number;
  class?: string;
  clipAbove?: number;
  focusedKey?: string;
  onFocusSection?: (key: string) => void;
  /** Read on past the focused section's line (text below the last row that takes focus). */
  extraShift?: number;
}) {
  const [focusedKey, setFocusedKey] = useState<string | null>(p.focusedKey ?? null);
  const top = p.top ?? 0;
  let y = top;
  const positions: { section: Section; y: number }[] = [];
  for (const s of p.sections) {
    positions.push({ section: s, y });
    y += s.height + ROW_GAP;
  }
  const at = positions.filter((x) => x.section.key === focusedKey)[0];
  let shift = 0;
  if (at && at.section.anchor !== null) shift = Math.max(0, at.y - at.section.anchor);
  shift += p.extraShift ?? 0;
  const clip = shift > 0 && p.clipAbove !== undefined ? `clip-path:inset(${p.clipAbove}px 0 0 0);-webkit-clip-path:inset(${p.clipAbove}px 0 0 0)` : "";
  const near = positions.filter(({ section, y: sy }) => sy + section.height >= shift - SCREEN && sy <= shift + 2 * SCREEN);
  return (
    <div class={p.class} style={`position:absolute;left:0;top:0;right:0;bottom:0;${clip}`}>
      <G kind="col" class="page-scroll" style={`transform:translateY(${-shift}px)`}>
        {near.map(({ section, y: sy }) => (
          <div key={section.key} style={`position:absolute;left:0;right:0;top:${sy}px;height:${section.height}px`}>
            {section.render(() => {
              setFocusedKey(section.key);
              p.onFocusSection?.(section.key);
            })}
          </div>
        ))}
      </G>
    </div>
  );
}
