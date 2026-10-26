import type { ComponentChildren } from "preact";
import { useRef, useState } from "preact/hooks";
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
  /** One tile, absolutely placed with `style`. */
  tile: (item: T, index: number, place: { style: string; onFocus: (el: HTMLElement) => void; priority: number }) => ComponentChildren;
  /** A "See all" tile after the items. */
  seeAll?: (place: { style: string; onFocus: (el: HTMLElement) => void }) => ComponentChildren;
  /** Left from the first tile: the rail. */
  onLeftEdge?: () => boolean;
  /** Where focus enters the row when it has none to come back to. */
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
 */
export function Row<T>(p: RowProps<T>) {
  const ui = useUi();
  const spec = TILES[p.kind];
  const count = p.items.length + (p.seeAll ? 1 : 0);
  const [focused, setFocused] = useState(p.enterAt ?? 0);
  const offset = rowOffset(p.kind, count, Math.min(focused, Math.max(0, count - 1)));
  const [first, last] = windowOf(p.kind, count, offset);
  const groupEl = useRef<HTMLElement | null>(null);
  const place = (i: number) => {
    const style = `position:absolute;left:${GUTTER + i * (spec.w + spec.gap)}px;top:0`;
    const onFocus = () => {
      setFocused(i);
      p.onFocusIndex?.(i);
    };
    return { style, onFocus, priority: Math.abs(i - focused) };
  };
  const onEdge = (dir: Dir) => {
    if (dir === "left") return p.onLeftEdge ? p.onLeftEdge() : false;
    return dir === "right";
  };
  const tiles: ComponentChildren[] = [];
  for (let i = first; i < Math.min(last, p.items.length); i++) tiles.push(p.tile(p.items[i]!, i, place(i)));
  if (p.seeAll && last >= count && first <= p.items.length) tiles.push(p.seeAll(place(p.items.length)));
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
          // A row never visited: its first tile, or the one asked for; else the default (nearest).
          if (!groupEl.current || ui.focus.rememberedIn(groupEl.current)) return null;
          if (p.enterAt !== undefined) return groupEl.current.querySelector<HTMLElement>(`[data-index="${p.enterAt}"] [data-f], [data-index="${p.enterAt}"][data-f]`);
          return from ? null : groupEl.current.querySelector<HTMLElement>("[data-f]");
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
 * scrolled nothing draws above the anchor line − 12.
 */
export function SectionPage(p: { sections: Section[]; top?: number; bottomRoom?: number; class?: string; clipAbove?: number; focusedKey?: string; onFocusSection?: (key: string) => void }) {
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
  const clip = shift > 0 && p.clipAbove !== undefined ? `clip-path:inset(${p.clipAbove}px 0 0 0);-webkit-clip-path:inset(${p.clipAbove}px 0 0 0)` : "";
  return (
    <div class={p.class} style={`position:absolute;left:0;top:0;right:0;bottom:0;${clip}`}>
      <G kind="col" class="page-scroll" style={`transform:translateY(${-shift}px)`}>
        {positions.map(({ section, y: sy }) => (
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
