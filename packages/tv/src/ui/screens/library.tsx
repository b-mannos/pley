import { useSignal, type Signal } from "@preact/signals";
import type { ComponentChildren } from "preact";
import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { Collection, Hub, ItemSummary, Library, LibraryKind, LibrarySort } from "../../api/types";
import { formatBitrate, formatBytes, pluralize } from "../../format/format";
import { useUi } from "../context";
import { Chip, PanelRow, Tab } from "../kit/buttons";
import { F, G } from "../kit/focusable";
import { gridColumns, PosterGrid } from "../kit/grid";
import { Icon, type IconName } from "../kit/icons";
import { useRested } from "../kit/backdrop";
import { PanelTitle, SidePanel } from "../kit/panel";
import { Row, rowHeight, SectionPage, type Section } from "../kit/row";
import { ArtTile, SeeAllTile, SkeletonTile, Tile, TILES, type TileKind } from "../kit/tile";
import type { Entry, LibraryTab } from "../shell/navigator";
import { ColumnMessage, failedTitle, problem, useApp, useLoad } from "./common";
import { pageOf, prefetchPlayback } from "./item-actions";
import {
  AlphaIndex,
  countLabel,
  defaultOrder,
  fitParams,
  grouped,
  hasFilter,
  LETTERS,
  Listing,
  ParamsStore,
  showsJump,
  smartFilterOf,
  SORT_LABELS,
  sortsFor,
  type LibraryParams,
} from "./library-data";

const HEADER_TOP = 27;
const CHIPS_TOP = 80;
/** The wells of a grid's focused row, below the chips. */
const GRID_TOP = 136;
const GRID_CLIP = 122;
/** Skeleton cells while the total isn't known: two rows and a bit. */
const UNKNOWN_CELLS = 18;

export function kindIcon(kind: LibraryKind): IconName {
  return kind === "movies" ? "clapperboard" : kind === "shows" ? "tv" : "music";
}

function tabsFor(kind: LibraryKind): LibraryTab[] {
  return kind === "music" ? ["recommended", "library"] : ["recommended", "library", "collections"];
}

const TAB_LABELS: Record<LibraryTab, string> = { recommended: "Recommended", library: "Library", collections: "Collections" };

/** "12 movies", "3 shows" for a collection. */
export function collectionCount(c: Collection, kind: LibraryKind | null): string {
  return kind === "shows" ? pluralize(c.item_count, "show") : pluralize(c.item_count, "movie");
}

/**
 * A library (TvLibraryScreen): Recommended rows, the whole library as a
 * grid with its sort, filters and A–Z, and its collections.
 */
export function LibraryScreen(p: { entry: Entry; id: number; tab?: LibraryTab }) {
  const app = useApp();
  const ui = useUi();
  const libraries = app.libraries.value;
  const library = libraries?.filter((l) => l.id === p.id)[0] ?? null;
  const entry = p.entry;
  const tabs = library ? tabsFor(library.kind) : ["library" as LibraryTab];
  // A library opens on the whole of it, as on Android; music's Collections is its Library.
  const wanted = (entry.state.tab as LibraryTab | undefined) ?? p.tab ?? "library";
  const shown: LibraryTab = tabs.indexOf(wanted) >= 0 ? wanted : "library";
  const opened = (entry.state.opened as LibraryTab | undefined) ?? shown;
  entry.state.opened = opened;
  const [, setTabState] = useState(0);
  const tabsRef = useRef<HTMLElement | null>(null);

  if (!libraries) return <LibrariesFailed />;
  if (!library) return <ColumnMessage title="This library isn't here" body="It may have been removed, or this profile may not see it." />;

  const showTab = (t: LibraryTab) => {
    entry.state.tab = t;
    setTabState((n) => n + 1);
  };
  const focusShownTab = () => {
    const el = tabsRef.current?.querySelector<HTMLElement>(`[data-fid="tab/${shown}"]`);
    return !!el && ui.focus.focus(el);
  };
  const initial = shown === opened;
  return (
    <div class="library-page">
      <div class="library-header">
        <div class="t-page c-fg one-line" style="flex:1;min-width:0">
          {library.name}
        </div>
        <G
          kind="row"
          class="library-tabs"
          elRef={(el) => (tabsRef.current = el)}
          onEnter={() => tabsRef.current?.querySelector<HTMLElement>(`[data-fid="tab/${shown}"]`) ?? null}
        >
          {tabs.map((t) => (
            <Tab key={t} fid={`tab/${t}`} label={TAB_LABELS[t]} shown={t === shown} onSelect={() => showTab(t)} />
          ))}
        </G>
      </div>
      {shown === "library" && <LibraryGridTab key={`grid-${library.id}`} entry={entry} library={library} initial={initial} focusTab={focusShownTab} />}
      {shown === "recommended" && (
        <RecommendedTab key={`rec-${library.id}`} entry={entry} library={library} initial={initial} focusTab={focusShownTab} onCollections={() => showTab("collections")} />
      )}
      {shown === "collections" && <CollectionsTab key={`col-${library.id}`} entry={entry} library={library} initial={initial} focusTab={focusShownTab} />}
    </div>
  );
}

// ---- the Library tab

type Panel = null | { kind: "sort" } | { kind: "filter" } | { kind: "genre"; from: "filter" | "chip" } | { kind: "year"; from: "filter" | "chip" };

function LibraryGridTab(p: { entry: Entry; library: Library; initial: boolean; focusTab: () => boolean }) {
  const app = useApp();
  const ui = useUi();
  const lib = p.library;
  const music = lib.kind === "music";
  const profile = app.me.value?.id ?? null;
  const store = useMemo(() => new ParamsStore(app.store, app.server.value ?? "", profile), [app.server.value, profile]);
  const [params, setParamsState] = useState<LibraryParams>(() => store.get(lib.id, lib.kind));
  const [version, setVersion] = useState(0);
  const listing = useMemo(
    () => new Listing(ui.api, lib.id, params, ui.tier === "low" ? 2000 : 5000, () => setVersion((v) => v + 1)),
    [lib.id, params.sort, params.order, params.watched, params.genre, params.year],
  );
  useEffect(() => {
    void listing.start();
    return () => listing.stop();
  }, [listing]);
  // The server's changes: the same listing asked again, what shows stays meanwhile.
  const changes = app.changes.value;
  const firstChanges = useRef(changes);
  useEffect(() => {
    if (changes !== firstChanges.current) void listing.start();
  }, [changes]);
  const filters = useLoad((c) => (music ? Promise.resolve({ genres: [], years: [] }) : ui.api.libraryFilters(lib.id, c)), [lib.id], { live: false });
  const [panel, setPanel] = useState<Panel>(null);
  const [jumpTo, setJumpTo] = useState<{ index: number; n: number } | null>(null);
  // Where focus is in the grid: only the A–Z jump reads it, so a move draws nothing else of the tab again.
  const focusedIndex = useSignal<number>((p.entry.state.gridIndex as number | undefined) ?? 0);
  const chipsRef = useRef<HTMLElement | null>(null);
  const panelOpener = useRef<string | null>(null);

  const setParams = (next: LibraryParams) => {
    const fitted = fitParams(next, lib.kind);
    store.set(lib.id, fitted);
    p.entry.state.gridIndex = 0;
    focusedIndex.value = 0;
    setParamsState(fitted);
  };

  const total = listing.total;
  const all = listing.all();
  const alpha = useMemo(() => (all ? new AlphaIndex(all) : null), [all ? all.length : -1, version, listing]);
  const jump = showsJump(params, total, listing.windowed) && !!alpha;
  const kind: TileKind = music ? "square" : "grid";
  const count = total ?? UNKNOWN_CELLS;
  const filterSet = hasFilter(params);
  void version;

  const closePanel = () => {
    setPanel(null);
    // Back to the chip that opened the first panel of the sequence.
    const fid = panelOpener.current;
    panelOpener.current = null;
    if (fid) {
      setTimeout(() => {
        const el = chipsRef.current?.querySelector<HTMLElement>(`[data-fid="${fid}"]`);
        if (el) ui.focus.focus(el);
      }, 0);
    }
  };
  const openPanel = (next: Panel, chip: string) => {
    if (!panelOpener.current) panelOpener.current = chip;
    setPanel(next);
  };

  const cell = (i: number, place: { onFocus: (el: HTMLElement) => void; priority: number }) => {
    const item = listing.at(i);
    if (!item) {
      return <SkeletonTile kind={kind} fid={`grid/cell${i}`} focusable onFocus={place.onFocus} />;
    }
    const bitrate = params.sort === "bitrate";
    return (
      <Tile
        item={item}
        kind={kind}
        fid={`grid/${item.id}`}
        initial={p.initial && i === 0}
        badges={bitrate}
        subtitle={bitrate ? bitrateLine(item) : undefined}
        priority={place.priority}
        onFocus={place.onFocus}
        onSelect={() => ui.nav.open(pageOf(item))}
        onActions={() => ui.nav.open({ name: "actions", id: item.id })}
      />
    );
  };

  let message: ComponentChildren = null;
  if (listing.error && total === null) {
    message = (
      <ColumnMessage
        top={146}
        title={failedTitle(listing.error, "Couldn't load this library")}
        body={problem(listing.error)}
        action={{ label: "Retry", icon: "refresh-cw", onSelect: () => setParams({ ...params }) }}
      />
    );
  } else if (total === 0 && filterSet) {
    message = (
      <ColumnMessage
        top={146}
        title="No matches"
        body="Nothing in this library matches the current filters."
        action={{ label: "Clear filters", onSelect: () => setParams({ ...params, watched: null, genre: null, year: null }) }}
      />
    );
  } else if (total === 0) {
    message = (
      <ColumnMessage
        top={146}
        title="This library is empty"
        body="pley hasn't found anything in it yet. Titles show up here as soon as the server adds them."
        focusInstead={p.focusTab}
        takeFocus={p.initial}
      />
    );
  }

  const genres = filters.data?.genres ?? [];
  const years = filters.data?.years ?? [];
  return (
    <>
      <G kind="row" class="library-chips" elRef={(el) => (chipsRef.current = el)} onEnter={(dir) => (dir === "down" ? chipsRef.current?.querySelector<HTMLElement>("[data-f]") ?? null : null)}>
        <Chip
          fid="chip/sort"
          label={SORT_LABELS[params.sort]}
          icon={params.order === "asc" ? "arrow-up-narrow-wide" : "arrow-down-wide-narrow"}
          onSelect={() => openPanel({ kind: "sort" }, "chip/sort")}
        />
        {!music && <Chip fid="chip/filter" label="Filter" icon="sliders-horizontal" onSelect={() => openPanel({ kind: "filter" }, "chip/filter")} />}
        {!music && (
          <Chip fid="chip/unwatched" label="Unwatched" selected={params.watched === false} onSelect={() => setParams({ ...params, watched: params.watched === false ? null : false })} />
        )}
        {!music && (genres.length > 0 || params.genre) && (
          <Chip fid="chip/genre" label={params.genre ?? "Genre"} selected={!!params.genre} maxWidth={220} onSelect={() => openPanel({ kind: "genre", from: "chip" }, "chip/genre")} />
        )}
        {!music && (years.length > 0 || params.year) && (
          <Chip fid="chip/year" label={params.year ? String(params.year) : "Year"} selected={!!params.year} onSelect={() => openPanel({ kind: "year", from: "chip" }, "chip/year")} />
        )}
        {total !== null && <div class="t-caption c-muted chips-count">{countLabel(total, lib.kind, filterSet && lib.item_count >= total ? lib.item_count : null)}</div>}
      </G>
      {message ?? (
        <PosterGrid
          key={`${params.sort}.${params.order}.${params.watched}.${params.genre}.${params.year}`}
          kind={kind}
          count={count}
          top={GRID_TOP}
          clipTop={GRID_CLIP}
          enterAt={0}
          startIndex={focusedIndex.peek()}
          focusTo={jumpTo}
          cell={cell}
          onFocusIndex={(i) => {
            focusedIndex.value = i;
            p.entry.state.gridIndex = i;
            listing.showing(Math.max(0, i - gridColumns(kind)));
          }}
          onRightEdge={
            jump
              ? () => {
                  const box = document.querySelector<HTMLElement>(".jump-box");
                  return !!box && ui.focus.focus(box);
                }
              : undefined
          }
        />
      )}
      {!message && <GridRest at={focusedIndex} itemAt={(i) => listing.at(i) ?? null} />}
      {jump && alpha && (
        <JumpControl
          alpha={alpha}
          focusedIndex={focusedIndex}
          onJump={(index) => setJumpTo({ index, n: Date.now() })}
          onLeft={() => {
            const g = document.querySelector<HTMLElement>(".grid-cells");
            const back = g ? ui.focus.rememberedIn(g) : null;
            return !!back && ui.focus.focus(back);
          }}
        />
      )}
      {panel?.kind === "sort" && (
        <SortPanel
          kind={lib.kind}
          params={params}
          onClose={closePanel}
          onPick={(next) => {
            setParams(next);
            closePanel();
          }}
        />
      )}
      {panel?.kind === "filter" && (
        <FilterPanel
          music={music}
          params={params}
          onClose={closePanel}
          onChange={(next) => setParams(next)}
          onGenre={() => setPanel({ kind: "genre", from: "filter" })}
          onYear={() => setPanel({ kind: "year", from: "filter" })}
          onClear={() => {
            setParams({ ...params, watched: null, genre: null, year: null });
            closePanel();
          }}
          onSaveAs={() => {
            setPanel(null);
            panelOpener.current = null;
            // Back from it lands on the Filter chip.
            p.entry.memory = "chip/filter";
            ui.nav.open({ name: "newPlaylist", smart: { libraryId: lib.id, filter: smartFilterOf(params) } });
          }}
        />
      )}
      {panel?.kind === "genre" && (
        <ChoicePanel
          title="Genre"
          all="All genres"
          choices={genres}
          current={params.genre}
          onClose={closePanel}
          onPick={(g) => {
            setParams({ ...params, genre: g });
            closePanel();
          }}
        />
      )}
      {panel?.kind === "year" && (
        <ChoicePanel
          title="Year"
          all="All years"
          choices={years.map(String)}
          current={params.year ? String(params.year) : null}
          onClose={closePanel}
          onPick={(y) => {
            setParams({ ...params, year: y ? Number(y) : null });
            closePanel();
          }}
        />
      )}
    </>
  );
}

/**
 * Where focus rests in the grid (400 ms, no key held): the title's details
 * come, and the server gets a video ready, as on Home's rows, so OK opens
 * its page whole and Play starts at once.
 */
function GridRest(p: { at: Signal<number>; itemAt: (index: number) => ItemSummary | null }) {
  const app = useApp();
  const rested = useRested(p.itemAt(p.at.value));
  useEffect(() => {
    if (!rested) return;
    prefetchPlayback(app, rested);
    // Music has pages of its own (an artist's, an album's), which ask for their own.
    const music = rested.kind === "artist" || rested.kind === "album" || rested.kind === "track";
    if (!music && !app.cachedItem(rested.id)) void app.item(rested.id).catch(() => undefined);
  }, [rested?.id]);
  return null;
}

/** "42 Mbps · 23.4 GB", or "Bitrate unknown". */
function bitrateLine(item: ItemSummary): string {
  const m = item.media;
  const parts = [m?.bitrate ? formatBitrate(m.bitrate) : "", m?.size ? formatBytes(m.size) : ""].filter(Boolean);
  return parts.length ? parts.join(" · ") : "Bitrate unknown";
}

/**
 * The A–Z jump (JumpControl): a box level with the grid's first row that
 * shows the focused title's letter; Up and Down pick another, OK goes there.
 */
function JumpControl(p: { alpha: AlphaIndex; focusedIndex: Signal<number>; onJump: (index: number) => void; onLeft: () => boolean }) {
  const [letter, setLetter] = useState(p.alpha.letterAt(p.focusedIndex.peek()));
  const [focused, setFocused] = useState(false);
  const shownLetter = focused ? letter : p.alpha.letterAt(p.focusedIndex.value);
  const at = LETTERS.indexOf(shownLetter);
  const slot = (l: string | undefined) => (l ? <span class={p.alpha.has(l) ? "c-muted" : "jump-empty"}>{l}</span> : null);
  return (
    <div class="jump">
      <div class="jump-slot t-caption">{focused ? slot(LETTERS[at - 1]) : null}</div>
      <F
        class="jump-box t-title"
        label={`Jump to a letter: ${shownLetter}`}
        onFocus={() => {
          setLetter(p.alpha.letterAt(p.focusedIndex.peek()));
          setFocused(true);
        }}
        onBlur={() => setFocused(false)}
        onSelect={() => p.onJump(p.alpha.jumpTarget(letter))}
        onKey={(key) => {
          if (key === "up") {
            const i = LETTERS.indexOf(letter);
            if (i <= 0) return false;
            setLetter(LETTERS[i - 1]!);
            return true;
          }
          if (key === "down") {
            const i = LETTERS.indexOf(letter);
            if (i < LETTERS.length - 1) setLetter(LETTERS[i + 1]!);
            return true;
          }
          if (key === "left") return p.onLeft();
          return key === "right";
        }}
      >
        {shownLetter}
      </F>
      <div class="jump-slot t-caption">{focused ? slot(LETTERS[at + 1]) : null}</div>
    </div>
  );
}

function SortPanel(p: { kind: LibraryKind; params: LibraryParams; onClose: () => void; onPick: (next: LibraryParams) => void }) {
  return (
    <SidePanel onClose={p.onClose} label="Sort by">
      <PanelTitle>Sort by</PanelTitle>
      {sortsFor(p.kind).map((s: LibrarySort) => (
        <PanelRow
          key={s}
          label={SORT_LABELS[s]}
          checked={s === p.params.sort}
          initial={s === p.params.sort}
          onSelect={() => p.onPick({ ...p.params, sort: s, order: defaultOrder(s) })}
        />
      ))}
      <div class="panel-heading-small t-caption c-muted">Order</div>
      <PanelRow label="Ascending" checked={p.params.order === "asc"} onSelect={() => p.onPick({ ...p.params, order: "asc" })} />
      <PanelRow label="Descending" checked={p.params.order === "desc"} onSelect={() => p.onPick({ ...p.params, order: "desc" })} />
    </SidePanel>
  );
}

function FilterPanel(p: {
  music: boolean;
  params: LibraryParams;
  onClose: () => void;
  onChange: (next: LibraryParams) => void;
  onGenre: () => void;
  onYear: () => void;
  onClear: () => void;
  /** Save these filters as a smart playlist (Save as…). */
  onSaveAs: () => void;
}) {
  const w = p.params.watched;
  const value = (text: string) => <span class="panel-value t-caption">{text}</span>;
  const chevron = <Icon name="chevron-right" size={20} class="check" />;
  return (
    <SidePanel onClose={p.onClose} label="Filter">
      <PanelTitle>Filter</PanelTitle>
      <div class="panel-heading-small t-caption c-muted">Show</div>
      <PanelRow label="All" checked={w === null} initial={w === null} onSelect={() => p.onChange({ ...p.params, watched: null })} />
      <PanelRow label="Unwatched" checked={w === false} initial={w === false} onSelect={() => p.onChange({ ...p.params, watched: false })} />
      <PanelRow label="Watched" checked={w === true} initial={w === true} onSelect={() => p.onChange({ ...p.params, watched: true })} />
      {!p.music && (
        <>
          <PanelRow label="Genre" trailing={<>{value(p.params.genre ?? "All genres")}{chevron}</>} onSelect={p.onGenre} />
          <PanelRow label="Year" trailing={<>{value(p.params.year ? String(p.params.year) : "All years")}{chevron}</>} onSelect={p.onYear} />
        </>
      )}
      {hasFilter(p.params) && <PanelRow label="Clear filters" onSelect={p.onClear} />}
      {hasFilter(p.params) && <PanelRow label="Save as…" icon="save" onSelect={p.onSaveAs} />}
    </SidePanel>
  );
}

function ChoicePanel(p: { title: string; all: string; choices: string[]; current: string | null; onClose: () => void; onPick: (v: string | null) => void }) {
  return (
    <SidePanel onClose={p.onClose} label={p.title}>
      <PanelTitle>{p.title}</PanelTitle>
      <PanelRow label={p.all} checked={p.current === null} initial={p.current === null} onSelect={() => p.onPick(null)} />
      {p.choices.map((c) => (
        <PanelRow key={c} label={c} checked={c === p.current} initial={c === p.current} onSelect={() => p.onPick(c)} />
      ))}
    </SidePanel>
  );
}

// ---- Recommended

function RecommendedTab(p: { entry: Entry; library: Library; initial: boolean; focusTab: () => boolean; onCollections: () => void }) {
  const ui = useUi();
  const lib = p.library;
  const music = lib.kind === "music";
  const hubs = useLoad((c) => ui.api.libraryHubs(lib.id, c), [lib.id]);
  const collections = useLoad((c) => (music ? Promise.resolve({ collections: [] as Collection[] }) : ui.api.collections(lib.id, c)), [lib.id]);
  const rows = (hubs.data?.hubs ?? []).filter((h) => h.items.length > 0);
  const cols = collections.data?.collections ?? [];
  const rowIndex = (p.entry.state.recIndex as Record<string, number> | undefined) ?? {};
  p.entry.state.recIndex = rowIndex;
  if (!hubs.data && hubs.error) {
    return <ColumnMessage top={CHIPS_TOP} title={failedTitle(hubs.error, "Couldn't load recommendations")} body={problem(hubs.error)} action={{ label: "Retry", icon: "refresh-cw", onSelect: hubs.reload }} />;
  }
  if (!hubs.data) return <RowsSkeleton top={CHIPS_TOP} />;
  if (!rows.length && !cols.length) {
    return (
      <ColumnMessage
        top={CHIPS_TOP}
        title="Nothing to recommend yet"
        body="Recommendations appear once the library has some items."
        focusInstead={p.focusTab}
        takeFocus={p.initial}
      />
    );
  }
  const sections: Section[] = rows.map((hub: Hub, h) => {
    const kind: TileKind = hub.key.indexOf("continue_watching") === 0 ? "landscape" : music ? "square" : "poster";
    return {
      key: hub.key,
      height: rowHeight(kind),
      anchor: CHIPS_TOP,
      render: (onFocus) => (
        <Row
          title={hub.title}
          kind={kind}
          items={hub.items}
          keyOf={(i) => i.id}
          enterAt={rowIndex[hub.key]}
          onLeftEdge={ui.toRail}
          onFocusIndex={(i) => {
            rowIndex[hub.key] = i;
            onFocus();
          }}
          tile={(item, i, place) => (
            <Tile
              item={item}
              kind={kind}
              fid={`${hub.key}/${item.id}`}
              initial={p.initial && h === 0 && i === 0}
              priority={place.priority}
              onFocus={place.onFocus}
              onSelect={() => ui.nav.open(pageOf(item))}
              onActions={() => ui.nav.open({ name: "actions", id: item.id, continueWatching: hub.key.indexOf("continue") === 0 })}
            />
          )}
        />
      ),
    };
  });
  if (cols.length) {
    sections.push({
      key: "collections",
      height: rowHeight("poster"),
      anchor: CHIPS_TOP,
      render: (onFocus) => (
        <Row
          title="Collections"
          kind="poster"
          items={cols}
          keyOf={(c) => c.id}
          enterAt={rowIndex.collections}
          onLeftEdge={ui.toRail}
          onFocusIndex={(i) => {
            rowIndex.collections = i;
            onFocus();
          }}
          tile={(c, i, place) => (
            <ArtTile
              kind="poster"
              title={c.title}
              subtitle={collectionCount(c, lib.kind)}
              candidates={[c.poster_url]}
              fid={`collections/${c.id}`}
              initial={p.initial && !rows.length && i === 0}
              priority={place.priority}
              onFocus={place.onFocus}
              onSelect={() => ui.nav.open({ name: "collection", id: c.id })}
            />
          )}
          seeAll={(place) => <SeeAllTile kind="poster" fid="collections/all" onFocus={place.onFocus} onSelect={p.onCollections} />}
        />
      ),
    });
  }
  return (
    <div class="library-rows">
      <SectionPage
        sections={sections}
        top={CHIPS_TOP}
        clipAbove={CHIPS_TOP - 12}
        focusedKey={p.entry.state.recSection as string | undefined}
        onFocusSection={(k) => (p.entry.state.recSection = k)}
      />
      <div class="rail-fade" />
    </div>
  );
}

/** Two rows of grey wells while a page's rows load. */
export function RowsSkeleton(p: { top: number; kind?: TileKind }) {
  const spec = TILES[p.kind ?? "poster"];
  const one = (y: number) => (
    <div style={`position:absolute;left:96px;top:${y}px`}>
      <div class="skeleton" style="width:180px;height:18px" />
      <div style="display:flex;margin-top:22px">
        {[0, 1, 2, 3, 4, 5, 6].map((i) => (
          <div key={i} class="skeleton" style={`width:${spec.w}px;height:${spec.h}px;border-radius:4px;margin-right:${spec.gap}px`} />
        ))}
      </div>
    </div>
  );
  return (
    <div aria-label="Loading" role="status">
      {one(p.top)}
      {one(p.top + 18 + 22 + spec.h + 28)}
    </div>
  );
}

// ---- Collections

function CollectionsTab(p: { entry: Entry; library: Library; initial: boolean; focusTab: () => boolean }) {
  const ui = useUi();
  const lib = p.library;
  const loaded = useLoad((c) => ui.api.collections(lib.id, c), [lib.id]);
  const list = loaded.data?.collections ?? null;
  if (!list && loaded.error) {
    return <ColumnMessage top={CHIPS_TOP} title={failedTitle(loaded.error, "Couldn't load collections")} body={problem(loaded.error)} action={{ label: "Retry", icon: "refresh-cw", onSelect: loaded.reload }} />;
  }
  if (!list) return null;
  if (!list.length) {
    const body =
      lib.kind === "movies" ? "Movies of the same TMDB collection are grouped here once two of them are in this library." : "There are no collections in this library yet.";
    return <ColumnMessage top={CHIPS_TOP} title="No collections yet" body={body} focusInstead={p.focusTab} takeFocus={p.initial} />;
  }
  return (
    <>
      <div class="t-caption c-muted collections-count">{`${grouped(list.length)} ${list.length === 1 ? "collection" : "collections"}`}</div>
      <PosterGrid
        kind="grid"
        count={list.length}
        top={GRID_TOP}
        clipTop={GRID_CLIP}
        startIndex={(p.entry.state.colIndex as number | undefined) ?? 0}
        onFocusIndex={(i) => (p.entry.state.colIndex = i)}
        cell={(i, place) => {
          const c = list[i]!;
          return (
            <ArtTile
              kind="grid"
              title={c.title}
              subtitle={collectionCount(c, lib.kind)}
              candidates={[c.poster_url]}
              badge={String(c.item_count)}
              fid={`collections/${c.id}`}
              initial={p.initial && i === 0}
              priority={place.priority}
              onFocus={place.onFocus}
              onSelect={() => ui.nav.open({ name: "collection", id: c.id })}
            />
          );
        }}
      />
    </>
  );
}

// ---- Libraries (the rail's entry with more than three)

/** "412 movies", "38 shows · Scanning…", "120 artists". */
export function libraryCountLine(l: Library): string {
  const base = l.kind === "music" ? pluralize(l.item_count, "artist") : l.kind === "shows" ? pluralize(l.item_count, "show") : pluralize(l.item_count, "movie");
  return l.scanning ? `${base} · Scanning…` : base;
}

/**
 * The libraries couldn't be had and none were saved (TvLibraries' failure):
 * why, and Retry; nothing while they are on their way.
 */
export function LibrariesFailed(p: { top?: number }) {
  const app = useApp();
  const error = app.accountError.value;
  if (error === null) return null;
  return (
    <ColumnMessage
      top={p.top}
      title={failedTitle(error, "Couldn't load your libraries")}
      body={problem(error)}
      action={{ label: "Retry", icon: "refresh-cw", onSelect: () => void app.loadAccount().catch(() => undefined) }}
    />
  );
}

export function LibrariesScreen(p: { entry: Entry }) {
  const app = useApp();
  const ui = useUi();
  const libraries = app.libraries.value;
  useEffect(() => {
    if (!libraries) void app.loadAccount().catch(() => undefined);
  }, []);
  const [shift, setShift] = useState(0);
  if (!libraries) return <LibrariesFailed />;
  if (!libraries.length) return <ColumnMessage title="No libraries yet" body="Ask the server owner to add a library." />;
  return (
    <div class="libraries-page">
      <div class="t-page c-fg" style={`position:absolute;left:96px;top:${HEADER_TOP}px`}>
        Libraries
      </div>
      <div class="libraries-viewport">
        <div class="scroll-y" style={`transform:translateY(${-shift}px)`}>
          <G kind="grid" columns={3} restore class="libraries-grid">
            {libraries.map((l, i) => (
              <F
                key={l.id}
                class="library-card"
                fid={`library/${l.id}`}
                initial={i === 0}
                label={`${l.name}, ${libraryCountLine(l)}`}
                onFocus={() => {
                  const row = Math.floor(i / 3);
                  setShift(Math.max(0, row * 96 - 96));
                  p.entry.state.lib = l.id;
                }}
                onSelect={() => ui.nav.open({ name: "library", id: l.id })}
              >
                <div class="library-card-icon">
                  <Icon name={kindIcon(l.kind)} size={20} />
                </div>
                <div style="flex:1;min-width:0;margin-left:14px">
                  <div class="t-title one-line">{l.name}</div>
                  <div class="t-caption one-line card-sub">{libraryCountLine(l)}</div>
                </div>
              </F>
            ))}
          </G>
        </div>
      </div>
    </div>
  );
}
