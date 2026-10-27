import type { CSSProperties } from "preact";
import { useEffect, useState } from "preact/hooks";
import type { ItemSummary } from "../../api/types";
import { useUi } from "../context";
import { placeholderGradient } from "../images";

/** Which picture of an item a place shows. */
export type ArtKind = "poster" | "thumb" | "backdrop";

/**
 * The addresses to try for an item's picture, best first (the Android
 * app's ArtworkImage): a poster falls back to the season's and the show's;
 * a still to the backdrop, an episode's to its show's.
 */
export function artCandidates(item: ItemSummary, kind: ArtKind): string[] {
  const out: string[] = [];
  const add = (u: string | null | undefined) => {
    if (u && out.indexOf(u) < 0) out.push(u);
  };
  if (kind === "poster") {
    add(item.poster_url);
    if (item.parent_id) add(`/api/items/${item.parent_id}/images/poster`);
    if (item.grandparent_id) add(`/api/items/${item.grandparent_id}/images/poster`);
  } else if (kind === "thumb") {
    if (item.kind === "movie") add(item.backdrop_url);
    add(item.thumb_url);
    if (item.kind === "episode" && item.grandparent_id) add(`/api/items/${item.grandparent_id}/images/backdrop`);
    if (item.kind !== "episode") add(item.backdrop_url);
  } else {
    add(item.backdrop_url);
    if (item.grandparent_id) add(`/api/items/${item.grandparent_id}/images/backdrop`);
    if (item.parent_id) add(`/api/items/${item.parent_id}/images/backdrop`);
  }
  return out;
}

/** The placeholder's padding, each side. */
const TITLE_PAD = 12;
/** A bold glyph's width in ems, about, for the title's longest word. */
const GLYPH_EM = 0.56;

/**
 * The size a placeholder's title is set in, in px: Title's 16, or smaller
 * so that its longest word fits across the tile whole ("Casablanca" on a
 * poster), down to 11; a word longer still breaks where it must.
 */
export function placeholderTitleSize(title: string, width: number): number {
  const longest = title.split(/\s+/).reduce((n, word) => Math.max(n, word.length), 0);
  const fits = (width - 2 * TITLE_PAD) / (longest * GLYPH_EM);
  return fits >= 16 ? 16 : Math.max(11, Math.floor(fits));
}

/**
 * One picture from its candidates, `width` canvas px wide: the first that
 * loads wins. Until then the title's gradient, with the title on it when
 * `title` says so. Fades in over 150 ms unless it was in memory already.
 */
export function Art(props: {
  candidates: string[];
  width: number;
  title: string;
  showTitle?: boolean;
  class?: string;
  style?: CSSProperties;
  /** The widest picture asked for, in px (backdrops: 1280). */
  cap?: number;
  priority?: number;
}) {
  const ui = useUi();
  const px = props.width * ui.scale;
  const urls = props.candidates.map((c) => ui.images.url(c, px, props.cap));
  const key = urls.join("|");
  const firstReady = urls.map((u) => ui.images.ready(u)).filter(Boolean)[0] ?? null;
  const [src, setSrc] = useState<{ key: string; url: string | null; instant: boolean }>({ key, url: firstReady, instant: !!firstReady });
  useEffect(() => {
    let release = () => {};
    let cancelled = false;
    const tryAt = (i: number) => {
      if (cancelled) return;
      const u = urls[i];
      if (!u) {
        setSrc({ key, url: null, instant: true });
        return;
      }
      if (ui.images.isMissing(u)) {
        tryAt(i + 1);
        return;
      }
      const ready = ui.images.ready(u);
      release = ui.images.load(
        u,
        (objectUrl) => {
          if (cancelled) return;
          if (objectUrl) setSrc({ key, url: objectUrl, instant: !!ready || ui.tier === "low" });
          else {
            release();
            tryAt(i + 1);
          }
        },
        props.priority ?? 0,
      );
    };
    tryAt(0);
    return () => {
      cancelled = true;
      release();
    };
  }, [key]);
  const shown = src.key === key ? src.url : firstReady;
  const instant = src.key === key ? src.instant : !!firstReady;
  // "loaded": the picture is in; "covered": its fade is over, so the placeholder can go.
  const [state, setState] = useState<{ url: string | null; loaded: boolean; covered: boolean }>({ url: null, loaded: false, covered: false });
  const loaded = state.url === shown && state.loaded;
  const covered = state.url === shown && state.covered;
  useEffect(() => {
    if (!loaded || covered) return;
    const t = setTimeout(() => setState((s) => (s.url === shown ? { ...s, covered: true } : s)), instant ? 0 : 160);
    return () => clearTimeout(t);
  }, [loaded, covered, shown]);
  const titleSize = props.showTitle ? placeholderTitleSize(props.title, props.width) : 16;
  return (
    <div class={props.class} style={props.style}>
      {!covered && (
        <div class="tile-placeholder" style={{ background: placeholderGradient(props.title) }}>
          {props.showTitle && (
            <div class="t-title lines-4" style={titleSize < 16 ? `font-size:${titleSize}px;line-height:${Math.round(titleSize * 1.375)}px` : undefined}>
              {props.title}
            </div>
          )}
        </div>
      )}
      {shown && (
        <img
          class={`tile-img${loaded ? " in" : ""}${instant ? " instant" : ""}`}
          src={shown}
          alt=""
          decoding="async"
          onLoad={() => setState({ url: shown, loaded: true, covered: false })}
        />
      )}
    </div>
  );
}
